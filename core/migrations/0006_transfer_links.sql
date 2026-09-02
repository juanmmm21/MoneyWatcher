-- Un traspaso entre dos cuentas propias son dos movimientos: la salida de una
-- y la entrada en la otra. Contados por separado inflan las dos columnas del
-- dashboard con dinero que nunca salió del patrimonio, así que se emparejan
-- para que las agregaciones puedan dejarlos fuera.
--
-- Esto no contradice el ADR 0005: no se inventa ningún saldo, solo se marca que
-- dos movimientos que la app ya tenía son las dos caras del mismo traspaso.
CREATE TABLE transfer_links (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    outgoing_id INTEGER NOT NULL UNIQUE REFERENCES transactions (id) ON DELETE CASCADE,
    incoming_id INTEGER NOT NULL UNIQUE REFERENCES transactions (id) ON DELETE CASCADE,
    -- Un enlace que el usuario descarta no se borra, se marca: borrarlo dejaría
    -- los dos movimientos libres y la siguiente detección volvería a proponer
    -- exactamente el mismo par.
    dismissed   INTEGER NOT NULL DEFAULT 0,
    detected_at TEXT    NOT NULL,
    -- Un movimiento no puede ser las dos caras del mismo traspaso.
    CHECK (outgoing_id <> incoming_id)
);

-- El UNIQUE de `outgoing_id` ya crea su índice; el de `incoming_id` también,
-- así que la búsqueda por cualquiera de los dos lados está cubierta.
