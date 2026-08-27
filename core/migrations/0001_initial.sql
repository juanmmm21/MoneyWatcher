CREATE TABLE accounts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT    NOT NULL,
    bank            TEXT    NOT NULL,
    kind            TEXT    NOT NULL,
    currency        TEXT    NOT NULL,
    opening_balance INTEGER NOT NULL DEFAULT 0,
    archived        INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT    NOT NULL,
    UNIQUE (bank, name)
);

CREATE TABLE categories (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    name      TEXT    NOT NULL,
    kind      TEXT    NOT NULL,
    color     TEXT    NOT NULL,
    is_system INTEGER NOT NULL DEFAULT 0,
    UNIQUE (name, kind)
);

CREATE TABLE imports (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id     INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    source_name    TEXT    NOT NULL,
    imported_at    TEXT    NOT NULL,
    imported_count INTEGER NOT NULL,
    duplicate_count INTEGER NOT NULL
);

-- Los importes se guardan como enteros (céntimos). SQLite no tiene decimal
-- exacto y REAL introduciría el error binario que el dominio evita.
CREATE TABLE transactions (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id    INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    booked_on     TEXT    NOT NULL,
    value_on      TEXT,
    description   TEXT    NOT NULL,
    counterparty  TEXT,
    amount        INTEGER NOT NULL,
    balance_after INTEGER,
    category_id   INTEGER REFERENCES categories (id) ON DELETE SET NULL,
    notes         TEXT,
    source        TEXT    NOT NULL,
    import_id     INTEGER REFERENCES imports (id) ON DELETE SET NULL,
    fingerprint   TEXT    NOT NULL,
    -- La deduplicación al reimportar un extracto solapado se apoya en este índice.
    UNIQUE (account_id, fingerprint)
);

CREATE INDEX idx_transactions_account_date ON transactions (account_id, booked_on);
CREATE INDEX idx_transactions_date ON transactions (booked_on);
CREATE INDEX idx_transactions_category ON transactions (category_id);

CREATE TABLE rules (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    matcher     TEXT    NOT NULL,
    pattern     TEXT    NOT NULL,
    account_id  INTEGER REFERENCES accounts (id) ON DELETE CASCADE,
    direction   TEXT,
    min_amount  INTEGER,
    max_amount  INTEGER,
    category_id INTEGER NOT NULL REFERENCES categories (id) ON DELETE CASCADE,
    priority    INTEGER NOT NULL DEFAULT 100,
    origin      TEXT    NOT NULL,
    hits        INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT    NOT NULL
);

CREATE INDEX idx_rules_priority ON rules (priority DESC, id ASC);

CREATE TABLE dashboard_widgets (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    kind     TEXT    NOT NULL,
    title    TEXT    NOT NULL,
    config   TEXT    NOT NULL,
    grid_x   INTEGER NOT NULL,
    grid_y   INTEGER NOT NULL,
    grid_w   INTEGER NOT NULL,
    grid_h   INTEGER NOT NULL
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
