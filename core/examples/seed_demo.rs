//! Genera una base de datos de demostración con movimientos inventados.
//!
//! Existe para poder enseñar la aplicación —capturas del README incluidas— sin
//! usar los extractos reales del usuario: ningún dato personal debe acabar en
//! una imagen publicada en un repositorio público.
//!
//! `cargo run -p moneywatcher-core --example seed_demo -- <directorio>`
//!
//! Después, la aplicación se arranca contra esa base con
//! `MONEYWATCHER_DATA_DIR=<directorio> npm run tauri dev`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Months, NaiveDate};
use moneywatcher_core::domain::{
    AccountId, AccountKind, CategoryId, ImportId, Money, NewAccount, NewRule, NewTransaction,
    RuleMatcher, RuleOrigin, TransactionSource,
};
use moneywatcher_core::rules::{apply_rules, LEARNED_RULE_PRIORITY, USER_RULE_PRIORITY};
use moneywatcher_core::storage::Database;

/// Meses de historia que se generan hacia atrás desde el mes en curso.
const MONTHS_OF_HISTORY: u32 = 14;

/// Semilla fija: la demo tiene que salir igual en cada máquina y en cada
/// ejecución, o dos capturas del mismo README enseñarían cifras distintas.
const SEED: u64 = 0x4d6f_6e65_7957_6100;

/// Generador determinista (xorshift64*), suficiente para repartir importes y
/// fechas sin arrastrar una dependencia de números aleatorios al núcleo.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Rng {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Entero en `[low, high]`, ambos incluidos.
    fn between(&mut self, low: i64, high: i64) -> i64 {
        if high <= low {
            return low;
        }
        let span = (high - low + 1) as u64;
        low + (self.next_u64() % span) as i64
    }

    /// Decide con una probabilidad expresada en porcentaje.
    fn chance(&mut self, percent: u64) -> bool {
        self.next_u64() % 100 < percent
    }
}

/// Cuenta de la demo. Los bancos son inventados a propósito.
struct DemoAccount {
    name: &'static str,
    bank: &'static str,
    kind: AccountKind,
    /// Nombre del extracto del que «vienen» sus movimientos: la demo registra
    /// la importación para que la app cuente la misma historia que contaría
    /// con extractos de verdad.
    statement: &'static str,
}

const ACCOUNTS: [DemoAccount; 3] = [
    DemoAccount {
        name: "Cuenta corriente",
        bank: "Banco Iberia",
        kind: AccountKind::Checking,
        statement: "banco-iberia-corriente.csv",
    },
    DemoAccount {
        name: "Ahorro",
        bank: "Norte Digital",
        kind: AccountKind::Savings,
        statement: "norte-digital-ahorro.csv",
    },
    DemoAccount {
        name: "Tarjeta",
        bank: "Caja Levante",
        kind: AccountKind::Credit,
        statement: "caja-levante-tarjeta.xlsx",
    },
];

/// Movimiento que se repite todos los meses el mismo día.
struct Monthly {
    account: usize,
    day: u32,
    description: &'static str,
    counterparty: &'static str,
    category: &'static str,
    /// Importe base en céntimos, con signo.
    cents: i64,
    /// Variación máxima en céntimos hacia arriba o hacia abajo.
    jitter: i64,
}

const MONTHLY: [Monthly; 9] = [
    Monthly {
        account: 0,
        day: 27,
        description: "NOMINA ESTUDIO MERIDIANO SL",
        counterparty: "Estudio Meridiano SL",
        category: "Nómina",
        cents: 235_000,
        jitter: 4_000,
    },
    Monthly {
        account: 0,
        day: 1,
        description: "RECIBO ALQUILER VIVIENDA C/ OLIVO 14",
        counterparty: "Patrimonial Olivo",
        category: "Vivienda",
        cents: -78_000,
        jitter: 0,
    },
    Monthly {
        account: 0,
        day: 8,
        description: "ELECTRICA DEL SUR - FACTURA LUZ",
        counterparty: "Eléctrica del Sur",
        category: "Suministros",
        cents: -6_900,
        jitter: 2_600,
    },
    Monthly {
        account: 0,
        day: 12,
        description: "FIBRA Y MOVIL TELECOM IBERIA",
        counterparty: "Telecom Iberia",
        category: "Suministros",
        cents: -4_490,
        jitter: 0,
    },
    Monthly {
        account: 2,
        day: 3,
        description: "PAGO SUSCRIPCION STREAMFLIX",
        counterparty: "Streamflix",
        category: "Suscripciones",
        cents: -1_299,
        jitter: 0,
    },
    Monthly {
        account: 2,
        day: 17,
        description: "PAGO SUSCRIPCION SONARA MUSICA",
        counterparty: "Sonara",
        category: "Suscripciones",
        cents: -1_099,
        jitter: 0,
    },
    Monthly {
        account: 2,
        day: 5,
        description: "CUOTA GIMNASIO ATLAS",
        counterparty: "Gimnasio Atlas",
        category: "Ocio",
        cents: -3_490,
        jitter: 0,
    },
    Monthly {
        account: 0,
        day: 20,
        description: "SEGURO SALUD VITALIS",
        counterparty: "Vitalis Seguros",
        category: "Salud",
        cents: -5_820,
        jitter: 0,
    },
    Monthly {
        account: 0,
        day: 15,
        description: "PRESTAMO COCHE CUOTA MENSUAL",
        counterparty: "Financiera Delta",
        category: "Transporte",
        cents: -21_500,
        jitter: 0,
    },
];

/// Gasto o ingreso suelto que aparece un número variable de veces al mes.
struct Occasional {
    account: usize,
    /// Conceptos entre los que se elige, tal y como los escribiría un banco.
    descriptions: &'static [(&'static str, &'static str)],
    category: &'static str,
    /// Veces al mes: mínimo y máximo.
    times: (i64, i64),
    /// Importe en céntimos, con signo: extremos del intervalo.
    cents: (i64, i64),
}

const OCCASIONAL: [Occasional; 8] = [
    Occasional {
        account: 2,
        descriptions: &[
            (
                "COMPRA TARJ. SUPERMERCADO LA HUERTA",
                "Supermercado La Huerta",
            ),
            ("COMPRA TARJ. MERCADO CENTRAL", "Mercado Central"),
            ("COMPRA TARJ. FRUTERIA EL NARANJO", "Frutería El Naranjo"),
        ],
        category: "Supermercado",
        times: (6, 11),
        cents: (-9_800, -1_450),
    },
    Occasional {
        account: 2,
        descriptions: &[
            ("COMPRA TARJ. BAR LA PARADA", "Bar La Parada"),
            ("COMPRA TARJ. TABERNA DEL PUERTO", "Taberna del Puerto"),
            ("COMPRA TARJ. CAFETERIA ALMENDRO", "Cafetería Almendro"),
        ],
        category: "Restaurantes",
        times: (3, 7),
        cents: (-4_600, -950),
    },
    Occasional {
        account: 2,
        descriptions: &[
            ("COMPRA TARJ. GASOLINERA KM 42", "Gasolinera Km 42"),
            ("RECARGA TARJETA TRANSPORTE URBANO", "Transporte Urbano"),
        ],
        category: "Transporte",
        times: (2, 4),
        cents: (-6_500, -1_800),
    },
    Occasional {
        account: 2,
        descriptions: &[
            ("COMPRA TARJ. LIBRERIA CAPITULO", "Librería Capítulo"),
            ("COMPRA TARJ. MODA ATLANTICA", "Moda Atlántica"),
            ("COMPRA TARJ. BAZAR NORTE", "Bazar Norte"),
        ],
        category: "Compras",
        times: (1, 4),
        cents: (-13_500, -1_600),
    },
    Occasional {
        account: 2,
        descriptions: &[
            ("COMPRA TARJ. CINE ASTORIA", "Cine Astoria"),
            ("ENTRADAS TEATRO ROMEA", "Teatro Romea"),
        ],
        category: "Ocio",
        times: (0, 3),
        cents: (-4_200, -800),
    },
    Occasional {
        account: 0,
        descriptions: &[("FARMACIA PLAZA MAYOR", "Farmacia Plaza Mayor")],
        category: "Salud",
        times: (0, 2),
        cents: (-7_000, -1_100),
    },
    Occasional {
        account: 0,
        descriptions: &[
            ("TRANSFERENCIA RECIBIDA PROYECTO WEB", "Cliente Marbella"),
            ("TRANSFERENCIA RECIBIDA DISENO MARCA", "Cliente Aurora"),
        ],
        category: "Freelance",
        times: (0, 2),
        cents: (25_000, 92_000),
    },
    Occasional {
        account: 0,
        descriptions: &[("ABONO DEVOLUCION COMPRA ONLINE", "Tienda Aurora")],
        category: "Devoluciones",
        times: (0, 1),
        cents: (1_200, 6_400),
    },
];

/// Regla de categorización que la demo trae ya creada.
struct DemoRule {
    pattern: &'static str,
    category: &'static str,
    origin: RuleOrigin,
}

/// Las reglas no se inventan sus aciertos: los movimientos que casan con estos
/// patrones entran sin categoría y es el motor de reglas de verdad el que los
/// clasifica, así que el contador de la interfaz enseña trabajo real.
const RULES: [DemoRule; 5] = [
    DemoRule {
        pattern: "NOMINA",
        category: "Nómina",
        origin: RuleOrigin::User,
    },
    DemoRule {
        pattern: "STREAMFLIX",
        category: "Suscripciones",
        origin: RuleOrigin::User,
    },
    DemoRule {
        pattern: "SUPERMERCADO LA HUERTA",
        category: "Supermercado",
        origin: RuleOrigin::Learned,
    },
    DemoRule {
        pattern: "GASOLINERA",
        category: "Transporte",
        origin: RuleOrigin::Learned,
    },
    DemoRule {
        pattern: "TABERNA DEL PUERTO",
        category: "Restaurantes",
        origin: RuleOrigin::Assistant,
    },
];

/// Traspaso mensual de la cuenta corriente a la de ahorro. Se genera como dos
/// movimientos con el mismo importe y signo opuesto, que es exactamente lo que
/// aparecería en los dos extractos.
const TRANSFER_DAY: u32 = 28;
const TRANSFER_CENTS: i64 = 30_000;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let Some(directory) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("uso: seed_demo <directorio donde crear la base de demostración>");
        std::process::exit(2);
    };

    let path = directory.join("moneywatcher.db");
    // Nunca se escribe sobre una base existente: la de la demo y la personal se
    // llaman igual, y equivocarse de directorio no puede costarle los datos a nadie.
    if path.exists() {
        return Err(format!(
            "{} ya existe: bórralo o elige otro directorio",
            path.display()
        )
        .into());
    }
    std::fs::create_dir_all(&directory)?;

    let mut database = Database::open(&path)?;
    let ledger = create_accounts(&database)?;
    let categories = category_index(&database)?;
    create_rules(&database, &categories)?;
    let transactions = generate(&ledger, &categories)?;
    let summary = database.insert_transactions(&transactions)?;
    close_imports(&database, &ledger, &transactions)?;
    let categorization = apply_rules(&mut database)?;

    report(
        &path,
        &transactions,
        summary.inserted,
        categorization.pending,
    );
    Ok(())
}

/// Cuentas de la demo con la importación que las alimenta.
struct Ledger {
    accounts: Vec<AccountId>,
    imports: Vec<ImportId>,
}

/// Cierra cada importación con el número de movimientos que le corresponde, que
/// es lo que la pantalla de ajustes enseña como importaciones recientes.
fn close_imports(
    database: &Database,
    ledger: &Ledger,
    transactions: &[NewTransaction],
) -> Result<(), Box<dyn std::error::Error>> {
    for (index, import_id) in ledger.imports.iter().enumerate() {
        let account_id = ledger.accounts[index];
        let count = transactions
            .iter()
            .filter(|transaction| transaction.account_id == account_id)
            .count();
        database.finish_import(*import_id, count, 0)?;
    }
    Ok(())
}

fn create_rules(
    database: &Database,
    categories: &HashMap<String, CategoryId>,
) -> Result<(), Box<dyn std::error::Error>> {
    for rule in &RULES {
        database.create_rule(&NewRule {
            matcher: RuleMatcher::Contains,
            pattern: rule.pattern.to_string(),
            account_id: None,
            direction: None,
            min_amount: None,
            max_amount: None,
            category_id: category(categories, rule.category)?,
            priority: match rule.origin {
                RuleOrigin::User => USER_RULE_PRIORITY,
                _ => LEARNED_RULE_PRIORITY,
            },
            origin: rule.origin,
        })?;
    }
    Ok(())
}

/// Un movimiento que casa con una regla entra sin categoría, para que sea la
/// regla la que lo clasifique.
fn covered_by_a_rule(description: &str) -> bool {
    RULES
        .iter()
        .any(|rule| description.to_uppercase().contains(rule.pattern))
}

fn create_accounts(database: &Database) -> Result<Ledger, Box<dyn std::error::Error>> {
    let mut ledger = Ledger {
        accounts: Vec::with_capacity(ACCOUNTS.len()),
        imports: Vec::with_capacity(ACCOUNTS.len()),
    };

    for account in &ACCOUNTS {
        let created = database.create_account(&NewAccount {
            name: account.name.to_string(),
            bank: account.bank.to_string(),
            kind: account.kind,
        })?;
        let import_id = database.create_import(created.id, account.statement)?;
        ledger.accounts.push(created.id);
        ledger.imports.push(import_id);
    }

    Ok(ledger)
}

fn category_index(
    database: &Database,
) -> Result<HashMap<String, CategoryId>, Box<dyn std::error::Error>> {
    let index: HashMap<String, CategoryId> = database
        .categories()?
        .into_iter()
        .map(|category| (category.name, category.id))
        .collect();
    Ok(index)
}

fn category(
    categories: &HashMap<String, CategoryId>,
    name: &str,
) -> Result<CategoryId, Box<dyn std::error::Error>> {
    categories
        .get(name)
        .copied()
        .ok_or_else(|| format!("la categoría «{name}» no está en la base de datos").into())
}

/// Primer día del mes que abre la historia de la demo.
fn first_month(today: NaiveDate) -> Option<NaiveDate> {
    today
        .with_day(1)?
        .checked_sub_months(Months::new(MONTHS_OF_HISTORY - 1))
}

/// Ajusta un día del mes al último disponible (el recibo del 31 en febrero).
fn day_in_month(month: NaiveDate, day: u32) -> Option<NaiveDate> {
    (1..=day)
        .rev()
        .find_map(|candidate| month.with_day(candidate))
}

fn generate(
    ledger: &Ledger,
    categories: &HashMap<String, CategoryId>,
) -> Result<Vec<NewTransaction>, Box<dyn std::error::Error>> {
    let today = chrono::Local::now().date_naive();
    let start = first_month(today).ok_or("no se pudo calcular el mes inicial de la demo")?;
    let mut rng = Rng::new(SEED);
    let mut transactions = Vec::new();

    for offset in 0..MONTHS_OF_HISTORY {
        let Some(month) = start.checked_add_months(Months::new(offset)) else {
            break;
        };

        for entry in &MONTHLY {
            let Some(date) = day_in_month(month, entry.day) else {
                continue;
            };
            if date > today {
                continue;
            }
            let amount = entry.cents + rng.between(-entry.jitter, entry.jitter);
            transactions.push(new_transaction(
                ledger,
                entry.account,
                date,
                entry.description,
                entry.counterparty,
                Money::from_minor_units(amount),
                category(categories, entry.category)?,
            ));
        }

        for entry in &OCCASIONAL {
            let times = rng.between(entry.times.0, entry.times.1);
            for _ in 0..times {
                let day = rng.between(1, 28) as u32;
                let Some(date) = day_in_month(month, day) else {
                    continue;
                };
                if date > today {
                    continue;
                }
                let (description, counterparty) =
                    entry.descriptions[(rng.next_u64() as usize) % entry.descriptions.len()];
                let amount = rng.between(entry.cents.0, entry.cents.1);
                transactions.push(new_transaction(
                    ledger,
                    entry.account,
                    date,
                    description,
                    counterparty,
                    Money::from_minor_units(amount),
                    category(categories, entry.category)?,
                ));
            }
        }

        if let Some(date) = day_in_month(month, TRANSFER_DAY) {
            if date <= today {
                let transfer = category(categories, "Traspaso")?;
                transactions.push(new_transaction(
                    ledger,
                    0,
                    date,
                    "TRASPASO A CUENTA DE AHORRO",
                    "Norte Digital",
                    Money::from_minor_units(-TRANSFER_CENTS),
                    transfer,
                ));
                transactions.push(new_transaction(
                    ledger,
                    1,
                    date,
                    "TRASPASO DESDE CUENTA CORRIENTE",
                    "Banco Iberia",
                    Money::from_minor_units(TRANSFER_CENTS),
                    transfer,
                ));
            }
        }

        // Alguna comisión suelta: son las que el usuario quiere ver de un
        // vistazo, y sin ellas la demo enseña un banco que no cobra nada.
        if rng.chance(35) {
            let day = rng.between(2, 26) as u32;
            if let Some(date) = day_in_month(month, day) {
                if date <= today {
                    let amount = rng.between(600, 1_500);
                    transactions.push(new_transaction(
                        ledger,
                        0,
                        date,
                        "COMISION MANTENIMIENTO CUENTA",
                        "Banco Iberia",
                        Money::from_minor_units(-amount),
                        category(categories, "Comisiones")?,
                    ));
                }
            }
        }
    }

    transactions.sort_by_key(|transaction| transaction.booked_on);
    Ok(transactions)
}

/// Los movimientos de la demo no llevan `balance_after`: la app registra
/// movimientos y ese campo solo existe para contrastar lo que trae un extracto.
fn new_transaction(
    ledger: &Ledger,
    account: usize,
    booked_on: NaiveDate,
    description: &str,
    counterparty: &str,
    amount: Money,
    category_id: CategoryId,
) -> NewTransaction {
    NewTransaction {
        account_id: ledger.accounts[account],
        booked_on,
        value_on: None,
        description: description.to_string(),
        counterparty: Some(counterparty.to_string()),
        amount,
        balance_after: None,
        category_id: if covered_by_a_rule(description) {
            None
        } else {
            Some(category_id)
        },
        notes: None,
        source: TransactionSource::Imported,
        import_id: Some(ledger.imports[account]),
    }
}

fn report(path: &Path, transactions: &[NewTransaction], inserted: usize, pending: usize) {
    let income: i64 = transactions
        .iter()
        .map(|transaction| transaction.amount.minor_units())
        .filter(|cents| *cents > 0)
        .sum();
    let expense: i64 = transactions
        .iter()
        .map(|transaction| transaction.amount.minor_units())
        .filter(|cents| *cents < 0)
        .sum();

    println!("base de datos : {}", path.display());
    println!("cuentas       : {}", ACCOUNTS.len());
    println!("reglas        : {}", RULES.len());
    println!("movimientos   : {inserted}");
    println!("sin categoría : {pending}");
    if let (Some(first), Some(last)) = (transactions.first(), transactions.last()) {
        println!("periodo       : {} → {}", first.booked_on, last.booked_on);
    }
    println!(
        "ingresos      : {}",
        Money::from_minor_units(income).to_decimal_string()
    );
    println!(
        "gastos        : {}",
        Money::from_minor_units(expense).to_decimal_string()
    );
    println!();
    println!("para abrir la app con estos datos:");
    println!(
        "  MONEYWATCHER_DATA_DIR={} npm run tauri dev",
        path.parent().unwrap_or(Path::new(".")).display()
    );
}
