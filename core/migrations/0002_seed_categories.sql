-- Categorías de arranque para que la app sea usable desde el primer import.
-- `is_system = 1` solo evita el borrado accidental; el usuario puede renombrarlas.
INSERT INTO categories (name, kind, color, is_system) VALUES
    ('Salary',          'income',   '#2f9e6f', 1),
    ('Freelance',       'income',   '#3fae86', 1),
    ('Investments',     'income',   '#5cc4a1', 1),
    ('Refunds',         'income',   '#7fd7bb', 1),
    ('Other income',    'income',   '#a3e3cf', 1),
    ('Groceries',       'expense',  '#d4694a', 1),
    ('Housing',         'expense',  '#c9523a', 1),
    ('Utilities',       'expense',  '#e08a5f', 1),
    ('Transport',       'expense',  '#e3a76a', 1),
    ('Health',          'expense',  '#cf6f8a', 1),
    ('Leisure',         'expense',  '#b06fb5', 1),
    ('Subscriptions',   'expense',  '#8a72c9', 1),
    ('Eating out',      'expense',  '#e0aa4f', 1),
    ('Shopping',        'expense',  '#c98fa0', 1),
    ('Taxes',           'expense',  '#8d8d8d', 1),
    ('Fees',            'expense',  '#a0a0a0', 1),
    ('Other expense',   'expense',  '#b5b5b5', 1),
    ('Transfer',        'transfer', '#6b8fd4', 1);
