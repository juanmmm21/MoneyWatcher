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
