use chrono::NaiveDate;

/// Interpretación día/mes elegida para todo el fichero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateOrder {
    DayFirst,
    MonthFirst,
    /// Formato ISO (`2026-03-14`) u otro no ambiguo.
    Unambiguous,
}

/// Decide de una vez cómo se leen las fechas del extracto completo.
///
/// Mezclar criterios fila a fila sería un desastre silencioso: `03/04` y `04/03`
/// se parsean sin error en ambos órdenes, así que se busca en toda la columna
/// alguna fecha que desempate (un número mayor que 12) y se aplica a todas.
pub fn detect_order(samples: &[String]) -> DateOrder {
    let mut saw_iso = false;

    for sample in samples {
        if parse_with_named_month(sample).is_some() {
            saw_iso = true;
            continue;
        }

        let Some(parts) = tokens(sample) else {
            continue;
        };

        if parts[0].len() == 4 {
            saw_iso = true;
            continue;
        }

        let first: u32 = parts[0].parse().unwrap_or(0);
        let second: u32 = parts[1].parse().unwrap_or(0);

        if first > 12 && second <= 12 {
            return DateOrder::DayFirst;
        }
        if second > 12 && first <= 12 {
            return DateOrder::MonthFirst;
        }
    }

    if saw_iso {
        DateOrder::Unambiguous
    } else {
        // Sin evidencia en contra se asume día primero: es lo que exportan los
        // bancos europeos, el público objetivo de la aplicación.
        DateOrder::DayFirst
    }
}

/// Parsea una fecha numérica. Se hace a mano en lugar de probar una lista de
/// formatos de `chrono` porque `%Y` acepta también años de dos dígitos, y eso
/// convierte `03/04/26` en el año 26 sin dar ningún error.
pub fn parse(raw: &str, order: DateOrder) -> Option<NaiveDate> {
    // Los extractos con el mes escrito ("12 feb 2026") no son ambiguos, así que
    // se resuelven aparte y sin mirar el orden deducido para el resto.
    if let Some(date) = parse_with_named_month(raw) {
        return Some(date);
    }

    let candidate = raw.split_whitespace().next()?;
    let parts = tokens(candidate)?;

    let (year, month, day) = if parts[0].len() == 4 {
        (parts[0], parts[1], parts[2])
    } else {
        match order {
            DateOrder::MonthFirst => (parts[2], parts[0], parts[1]),
            DateOrder::DayFirst | DateOrder::Unambiguous => (parts[2], parts[1], parts[0]),
        }
    };

    let year = expand_year(year)?;
    NaiveDate::from_ymd_opt(year, month.parse().ok()?, day.parse().ok()?)
}

/// Años de dos dígitos: se usa la convención POSIX (69-99 son del siglo XX),
/// que cubre tanto extractos antiguos como los actuales.
fn expand_year(raw: &str) -> Option<i32> {
    let value: i32 = raw.parse().ok()?;
    match raw.len() {
        4 => Some(value),
        2 => Some(if value >= 69 {
            1900 + value
        } else {
            2000 + value
        }),
        _ => None,
    }
}

/// Meses escritos, en español e inglés, por las tres primeras letras: cubre
/// tanto la forma abreviada ("feb", "sept") como la completa ("febrero").
/// Revolut y varios agregadores exportan así.
fn month_from_name(token: &str) -> Option<u32> {
    let key: String = token
        .chars()
        .filter(|c| c.is_alphabetic())
        .take(3)
        .flat_map(|c| fold_accent(c).to_lowercase())
        .collect();

    let month = match key.as_str() {
        "ene" | "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "abr" | "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "ago" | "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dic" | "dec" => 12,
        _ => return None,
    };
    Some(month)
}

fn fold_accent(ch: char) -> char {
    match ch {
        'á' | 'à' => 'a',
        'é' | 'è' => 'e',
        'í' | 'ì' => 'i',
        'ó' | 'ò' => 'o',
        'ú' | 'ù' | 'ü' => 'u',
        other => other,
    }
}

/// Fechas con el mes escrito: "12 feb 2026", "12-feb-2026", "Feb 12, 2026".
/// El día y el año se distinguen por magnitud, así que no hacen falta reglas de
/// orden: el número de cuatro cifras (o el mayor de 31) es el año.
fn parse_with_named_month(raw: &str) -> Option<NaiveDate> {
    let mut month = None;
    let mut numbers: Vec<&str> = Vec::new();

    for token in raw.split(|c: char| !c.is_alphanumeric()) {
        if token.is_empty() {
            continue;
        }
        if token.chars().all(|c| c.is_ascii_digit()) {
            numbers.push(token);
        } else if month.is_none() {
            month = Some(month_from_name(token)?);
        } else {
            // Una segunda palabra (una hora con "AM", una zona horaria) ya no
            // forma parte de la fecha.
            break;
        }
    }

    let month = month?;
    if numbers.len() < 2 {
        return None;
    }

    let (day, year) = if numbers[0].len() == 4 {
        (numbers[1], numbers[0])
    } else {
        (numbers[0], numbers[1])
    };

    NaiveDate::from_ymd_opt(expand_year(year)?, month, day.parse().ok()?)
}

/// Grupos de dígitos de la cadena, si hay al menos tres (día, mes y año).
fn tokens(sample: &str) -> Option<[&str; 3]> {
    let mut parts = sample
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty());

    let first = parts.next()?;
    let second = parts.next()?;
    let third = parts.next()?;
    Some([first, second, third])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn detects_day_first_from_a_day_above_twelve() {
        let order = detect_order(&strings(&["03/04/2026", "25/04/2026"]));
        assert_eq!(order, DateOrder::DayFirst);
        assert_eq!(
            parse("03/04/2026", order),
            NaiveDate::from_ymd_opt(2026, 4, 3)
        );
    }

    #[test]
    fn detects_month_first_from_us_style_column() {
        let order = detect_order(&strings(&["04/03/2026", "04/25/2026"]));
        assert_eq!(order, DateOrder::MonthFirst);
        assert_eq!(
            parse("04/03/2026", order),
            NaiveDate::from_ymd_opt(2026, 4, 3)
        );
    }

    #[test]
    fn detects_iso_dates() {
        let order = detect_order(&strings(&["2026-04-03", "2026-04-25"]));
        assert_eq!(order, DateOrder::Unambiguous);
        assert_eq!(
            parse("2026-04-03", order),
            NaiveDate::from_ymd_opt(2026, 4, 3)
        );
    }

    #[test]
    fn defaults_to_day_first_when_ambiguous() {
        assert_eq!(detect_order(&strings(&["03/04/2026"])), DateOrder::DayFirst);
    }

    #[test]
    fn parses_two_digit_years_and_trailing_time() {
        let order = DateOrder::DayFirst;
        assert_eq!(
            parse("03/04/26", order),
            NaiveDate::from_ymd_opt(2026, 4, 3)
        );
        assert_eq!(
            parse("03/04/2026 18:22", order),
            NaiveDate::from_ymd_opt(2026, 4, 3)
        );
        assert_eq!(parse("   ", order), None);
    }
}

#[cfg(test)]
mod named_month_tests {
    use super::*;

    #[test]
    fn parses_spanish_abbreviated_months() {
        // Formato de los extractos de Revolut en español.
        for (raw, expected) in [
            ("12 feb 2026", (2026, 2, 12)),
            ("1 ene 2026", (2026, 1, 1)),
            ("30 sept 2025", (2025, 9, 30)),
            ("5 dic 2025", (2025, 12, 5)),
            ("3 mar 2024", (2024, 3, 3)),
        ] {
            assert_eq!(
                parse(raw, DateOrder::DayFirst),
                NaiveDate::from_ymd_opt(expected.0, expected.1, expected.2),
                "no se pudo leer {raw}"
            );
        }
    }

    #[test]
    fn parses_full_and_accented_month_names() {
        assert_eq!(
            parse("12 febrero 2026", DateOrder::DayFirst),
            NaiveDate::from_ymd_opt(2026, 2, 12)
        );
        assert_eq!(
            parse("14 marzo 2026", DateOrder::DayFirst),
            NaiveDate::from_ymd_opt(2026, 3, 14)
        );
    }

    #[test]
    fn parses_english_month_first_and_separators() {
        assert_eq!(
            parse("Feb 12, 2026", DateOrder::DayFirst),
            NaiveDate::from_ymd_opt(2026, 2, 12)
        );
        assert_eq!(
            parse("12-feb-2026", DateOrder::DayFirst),
            NaiveDate::from_ymd_opt(2026, 2, 12)
        );
    }

    /// El orden día/mes se deduce de la columna entera, y una fecha con el mes
    /// escrito no puede votar en esa decisión porque no es ambigua.
    #[test]
    fn named_months_do_not_skew_the_detected_order() {
        let samples = vec!["12 feb 2026".to_string(), "3 mar 2026".to_string()];
        assert_eq!(detect_order(&samples), DateOrder::Unambiguous);
    }

    #[test]
    fn ignores_text_that_is_not_a_date() {
        assert_eq!(parse("Extracto de transaccion", DateOrder::DayFirst), None);
        assert_eq!(parse("Cuenta personal (EUR)", DateOrder::DayFirst), None);
        assert_eq!(parse("", DateOrder::DayFirst), None);
    }
}
