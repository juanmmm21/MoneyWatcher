use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub};

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

/// Número de decimales con el que trabaja la aplicación. Las divisas que maneja
/// MoneyWatcher (EUR, USD, GBP...) usan dos, y fijarlo permite representar todo
/// importe como un entero exacto en lugar de un flotante binario.
pub const SCALE: u32 = 2;
const MINOR_UNITS_PER_UNIT: i64 = 100;

/// Importe monetario almacenado en unidades menores (céntimos).
///
/// Nunca se usa coma flotante para dinero: los errores de representación de
/// `f64` se acumulan en las agregaciones (sumas mensuales, balances) y acaban
/// mostrando un céntimo de diferencia respecto al extracto del banco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Money(i64);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MoneyParseError {
    #[error("empty amount")]
    Empty,
    #[error("invalid amount `{0}`")]
    Invalid(String),
    #[error("amount `{0}` has more than {SCALE} decimal places")]
    TooManyDecimals(String),
    #[error("amount `{0}` is out of range")]
    OutOfRange(String),
}

impl Money {
    pub const ZERO: Money = Money(0);

    pub const fn from_minor_units(minor: i64) -> Self {
        Money(minor)
    }

    pub const fn minor_units(self) -> i64 {
        self.0
    }

    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn abs(self) -> Self {
        Money(self.0.saturating_abs())
    }

    /// Parsea importes tal y como aparecen en los extractos bancarios reales:
    /// `"1.234,56"`, `"1,234.56"`, `"-1234.56"`, `"(45,00)"`, `"1.234,56 €"`,
    /// `"EUR 12,00"` o `"12,00-"` (signo pospuesto, típico de exportaciones SEPA).
    pub fn parse_flexible(raw: &str) -> Result<Self, MoneyParseError> {
        let mut cleaned = String::with_capacity(raw.len());
        let mut negative = false;

        for ch in raw.chars() {
            match ch {
                '0'..='9' | '.' | ',' => cleaned.push(ch),
                '-' | '\u{2212}' => negative = true, // U+2212 es el menos tipográfico
                '(' => negative = true,
                // Todo lo demás es ruido del extracto (símbolos y códigos de
                // divisa, espacios finos, apóstrofos de millar): se descarta.
                // La divisa la define la cuenta, no la celda del extracto.
                _ => {}
            }
        }

        if cleaned.is_empty() {
            return Err(MoneyParseError::Empty);
        }

        let (integer_part, decimal_part) = split_separators(&cleaned, raw)?;

        if decimal_part.len() > SCALE as usize {
            return Err(MoneyParseError::TooManyDecimals(raw.to_string()));
        }

        let units: i64 = if integer_part.is_empty() {
            0
        } else {
            integer_part
                .parse()
                .map_err(|_| MoneyParseError::OutOfRange(raw.to_string()))?
        };

        let mut padded = decimal_part.to_string();
        while padded.len() < SCALE as usize {
            padded.push('0');
        }
        let minor_fraction: i64 = if padded.is_empty() {
            0
        } else {
            padded
                .parse()
                .map_err(|_| MoneyParseError::Invalid(raw.to_string()))?
        };

        let minor = units
            .checked_mul(MINOR_UNITS_PER_UNIT)
            .and_then(|v| v.checked_add(minor_fraction))
            .ok_or_else(|| MoneyParseError::OutOfRange(raw.to_string()))?;

        Ok(Money(if negative { -minor } else { minor }))
    }

    /// Representación decimal exacta (`"-1234.56"`), que es como los importes
    /// cruzan la frontera hacia el frontend para no volver a ser `number`.
    pub fn to_decimal_string(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        let units = abs / MINOR_UNITS_PER_UNIT as u64;
        let fraction = abs % MINOR_UNITS_PER_UNIT as u64;
        format!("{sign}{units}.{fraction:02}")
    }
}

/// Decide qué separador de la cadena limpia es el decimal. Los extractos
/// españoles usan `1.234,56` y los anglosajones `1,234.56`, así que el criterio
/// es la posición: el último separador manda, y solo es decimal si le siguen
/// como mucho dos dígitos.
fn split_separators<'a>(cleaned: &'a str, raw: &str) -> Result<(String, &'a str), MoneyParseError> {
    let last_sep = cleaned.rfind([',', '.']);

    match last_sep {
        None => Ok((cleaned.to_string(), "")),
        Some(idx) => {
            let tail = &cleaned[idx + 1..];
            if tail.contains([',', '.']) {
                return Err(MoneyParseError::Invalid(raw.to_string()));
            }
            if !tail.chars().all(|c| c.is_ascii_digit()) {
                return Err(MoneyParseError::Invalid(raw.to_string()));
            }

            // Tres dígitos tras el último separador: es un separador de miles
            // (`1.234`), no un decimal. Con 1 o 2 dígitos, es decimal.
            let head: String = cleaned[..idx]
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            if tail.len() == 3 && !head.is_empty() {
                let mut integer = head;
                integer.push_str(tail);
                Ok((integer, ""))
            } else if tail.len() > SCALE as usize {
                // Hay exportaciones (N26, Trade Republic) que rellenan la parte
                // decimal con ceros hasta seis o nueve cifras: `12.710000000`
                // es 12,71 exacto y tirar esa fila descuadra el extracto. Un
                // dígito significativo de más sí sigue siendo un error, porque
                // la única salida sería redondear, o sea inventarse el importe.
                let (kept, extra) = tail.split_at(SCALE as usize);
                if extra.chars().all(|digit| digit == '0') {
                    Ok((head, kept))
                } else {
                    Err(MoneyParseError::TooManyDecimals(raw.to_string()))
                }
            } else {
                Ok((head, tail))
            }
        }
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_decimal_string())
    }
}

impl Add for Money {
    type Output = Money;
    fn add(self, rhs: Money) -> Money {
        Money(self.0.saturating_add(rhs.0))
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Money) {
        self.0 = self.0.saturating_add(rhs.0);
    }
}

impl Sub for Money {
    type Output = Money;
    fn sub(self, rhs: Money) -> Money {
        Money(self.0.saturating_sub(rhs.0))
    }
}

impl Neg for Money {
    type Output = Money;
    fn neg(self) -> Money {
        Money(self.0.saturating_neg())
    }
}

impl Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, |acc, m| acc + m)
    }
}

impl Serialize for Money {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_decimal_string())
    }
}

impl<'de> Deserialize<'de> for Money {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Money::parse_flexible(&raw).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spanish_format() {
        assert_eq!(
            Money::parse_flexible("1.234,56").unwrap().minor_units(),
            123_456
        );
        assert_eq!(
            Money::parse_flexible("-1.234,56").unwrap().minor_units(),
            -123_456
        );
        assert_eq!(Money::parse_flexible("0,05").unwrap().minor_units(), 5);
    }

    #[test]
    fn parses_anglo_format() {
        assert_eq!(
            Money::parse_flexible("1,234.56").unwrap().minor_units(),
            123_456
        );
        assert_eq!(
            Money::parse_flexible("-1234.56").unwrap().minor_units(),
            -123_456
        );
    }

    #[test]
    fn parses_currency_noise_and_sign_variants() {
        assert_eq!(
            Money::parse_flexible("1.234,56 €").unwrap().minor_units(),
            123_456
        );
        assert_eq!(
            Money::parse_flexible("EUR 12,00").unwrap().minor_units(),
            1_200
        );
        assert_eq!(
            Money::parse_flexible("12,00-").unwrap().minor_units(),
            -1_200
        );
        assert_eq!(
            Money::parse_flexible("(45,00)").unwrap().minor_units(),
            -4_500
        );
        assert_eq!(
            Money::parse_flexible("\u{2212}45,00")
                .unwrap()
                .minor_units(),
            -4_500
        );
    }

    #[test]
    fn treats_lone_thousands_separator_as_thousands() {
        assert_eq!(
            Money::parse_flexible("1.234").unwrap().minor_units(),
            123_400
        );
        assert_eq!(
            Money::parse_flexible("1,234").unwrap().minor_units(),
            123_400
        );
        assert_eq!(
            Money::parse_flexible("1234").unwrap().minor_units(),
            123_400
        );
    }

    #[test]
    fn parses_amounts_without_integer_part() {
        assert_eq!(Money::parse_flexible(",50").unwrap().minor_units(), 50);
        assert_eq!(Money::parse_flexible("-.5").unwrap().minor_units(), -50);
    }

    /// Formato de N26 y de Trade Republic: el importe llega con la parte
    /// decimal rellena de ceros. Rechazarlo dejaba fuera 158 de 1.674 líneas de
    /// un extracto real, y con ellas el saldo dejaba de cuadrar.
    #[test]
    fn accepts_decimals_padded_with_zeros() {
        assert_eq!(
            Money::parse_flexible("12.710000000").unwrap().minor_units(),
            1_271
        );
        assert_eq!(
            Money::parse_flexible("-170.000000000")
                .unwrap()
                .minor_units(),
            -17_000
        );
        assert_eq!(Money::parse_flexible("0.050000").unwrap().minor_units(), 5);
        assert_eq!(
            Money::parse_flexible("1234,560000").unwrap().minor_units(),
            123_456
        );
    }

    /// Un céntimo partido no se redondea: redondear es inventarse el importe,
    /// y el usuario tiene que enterarse de que esa línea no ha entrado.
    #[test]
    fn still_rejects_significant_extra_decimals() {
        assert!(matches!(
            Money::parse_flexible("12.715000"),
            Err(MoneyParseError::TooManyDecimals(_))
        ));
        assert!(matches!(
            Money::parse_flexible("1.234567"),
            Err(MoneyParseError::TooManyDecimals(_))
        ));
        assert!(matches!(
            Money::parse_flexible("0,0001"),
            Err(MoneyParseError::TooManyDecimals(_))
        ));
    }

    #[test]
    fn rejects_invalid_amounts() {
        assert!(matches!(
            Money::parse_flexible(""),
            Err(MoneyParseError::Empty)
        ));
        assert!(matches!(
            Money::parse_flexible("   "),
            Err(MoneyParseError::Empty)
        ));
        assert!(matches!(
            Money::parse_flexible("12,3456"),
            Err(MoneyParseError::TooManyDecimals(_))
        ));
    }

    #[test]
    fn formats_with_two_decimals() {
        assert_eq!(
            Money::from_minor_units(-123_456).to_decimal_string(),
            "-1234.56"
        );
        assert_eq!(Money::from_minor_units(5).to_decimal_string(), "0.05");
        assert_eq!(Money::ZERO.to_decimal_string(), "0.00");
    }

    #[test]
    fn arithmetic_is_exact() {
        // La suma de 0,10 diez veces da exactamente 1,00, cosa que con f64 no ocurre.
        let total: Money = std::iter::repeat_n(Money::from_minor_units(10), 10).sum();
        assert_eq!(total.minor_units(), 100);
    }

    #[test]
    fn round_trips_through_serde() {
        let original = Money::from_minor_units(-98_765);
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, "\"-987.65\"");
        assert_eq!(serde_json::from_str::<Money>(&json).unwrap(), original);
    }
}
