use crate::domain::normalize_description;

/// Palabras que aparecen en casi todos los conceptos bancarios y no identifican
/// a nadie: si el patrón aprendido fuese "compra" o "recibo", la regla se
/// aplicaría a media cuenta.
const NOISE: &[&str] = &[
    "compra", "compras", "tarj", "tarjeta", "pago", "pagos", "recibo", "recibos", "domiciliado",
    "domiciliacion", "transferencia", "transf", "adeudo", "cargo", "abono", "ingreso", "efectivo",
    "operacion", "movimiento", "concepto", "ref", "referencia", "mandato", "orden", "favor",
    "cuenta", "banco", "oficina", "comision", "liquidacion", "fecha", "importe", "del", "de", "la",
    "el", "los", "las", "en", "por", "para", "con", "sin", "sl", "sa", "sau", "slu", "sociedad",
    "card", "payment", "purchase", "direct", "debit", "credit", "transfer", "from", "the", "and",
    "ltd", "llc", "inc", "plc", "gmbh", "dd", "sepa", "bizum", "atm",
];

/// Propone el patrón de una regla a partir del concepto de un movimiento que el
/// usuario acaba de categorizar a mano.
///
/// Se queda con la primera palabra que identifique de verdad al comercio,
/// descartando ruido y números (importes, números de tarjeta, referencias).
pub fn suggest_pattern(description: &str, counterparty: Option<&str>) -> Option<String> {
    if let Some(counterparty) = counterparty.filter(|value| !value.trim().is_empty()) {
        if let Some(pattern) = significant_token(counterparty) {
            return Some(pattern);
        }
    }

    significant_token(description)
}

fn significant_token(text: &str) -> Option<String> {
    let normalized = normalize_description(text);
    let mut candidates = normalized
        .split(' ')
        .filter(|token| token.len() >= 3)
        .filter(|token| !token.chars().all(|c| c.is_ascii_digit()))
        .filter(|token| !NOISE.contains(token));

    let first = candidates.next()?;

    // Un token corto rara vez identifica a un comercio; se le pega el siguiente
    // para que el patrón sea lo bastante específico ("club" -> "club natacion").
    if first.len() < 5 {
        if let Some(second) = candidates.next() {
            return Some(format!("{first} {second}"));
        }
    }

    Some(first.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_merchant_from_a_card_purchase() {
        assert_eq!(
            suggest_pattern("COMPRA TARJ. *1234 MERCADONA VALENCIA", None).as_deref(),
            Some("mercadona")
        );
    }

    #[test]
    fn prefers_the_counterparty_when_available() {
        assert_eq!(
            suggest_pattern("RECIBO DOMICILIADO", Some("IBERDROLA CLIENTES SAU")).as_deref(),
            Some("iberdrola")
        );
    }

    #[test]
    fn joins_two_tokens_when_the_first_is_too_short() {
        assert_eq!(
            suggest_pattern("PAGO CLUB NATACION MUNICIPAL", None).as_deref(),
            Some("club natacion")
        );
    }

    #[test]
    fn returns_nothing_when_everything_is_noise() {
        assert_eq!(suggest_pattern("PAGO TARJETA 1234", None), None);
        assert_eq!(suggest_pattern("   ", None), None);
    }
}
