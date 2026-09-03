-- Caché de lo que se ha averiguado de una marca consultándola fuera.
--
-- Existe para no repetir la consulta: cada llamada de red es una filtración
-- potencial, y lo que es Mercadona no cambia de un día para otro. Un `summary`
-- nulo significa «consultado y no hay respuesta útil», que también hay que
-- recordar para no volver a preguntar por lo mismo cada vez.
CREATE TABLE brand_lookups (
    term         TEXT PRIMARY KEY,
    summary      TEXT,
    looked_up_at TEXT NOT NULL
);
