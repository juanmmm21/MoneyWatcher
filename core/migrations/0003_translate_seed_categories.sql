-- Las categorías de arranque salieron en inglés mientras el resto de la
-- interfaz está en español. Se renombran en lugar de recrearlas para no perder
-- los movimientos ya categorizados ni las reglas que apuntan a ellas.
--
-- Solo se tocan las de sistema (`is_system = 1`): una categoría creada por el
-- usuario es suya, aunque coincida de nombre. El `UNIQUE (name, kind)` obliga a
-- ceder si el usuario ya tiene una con el nombre nuevo, así que cada renombrado
-- comprueba antes que el hueco está libre.
UPDATE categories SET name = 'Nómina'
 WHERE name = 'Salary' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Nómina' AND c.kind = 'income');

UPDATE categories SET name = 'Freelance'
 WHERE name = 'Freelance' AND is_system = 1;

UPDATE categories SET name = 'Inversiones'
 WHERE name = 'Investments' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Inversiones' AND c.kind = 'income');

UPDATE categories SET name = 'Devoluciones'
 WHERE name = 'Refunds' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Devoluciones' AND c.kind = 'income');

UPDATE categories SET name = 'Otros ingresos'
 WHERE name = 'Other income' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Otros ingresos' AND c.kind = 'income');

UPDATE categories SET name = 'Supermercado'
 WHERE name = 'Groceries' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Supermercado' AND c.kind = 'expense');

UPDATE categories SET name = 'Vivienda'
 WHERE name = 'Housing' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Vivienda' AND c.kind = 'expense');

UPDATE categories SET name = 'Suministros'
 WHERE name = 'Utilities' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Suministros' AND c.kind = 'expense');

UPDATE categories SET name = 'Transporte'
 WHERE name = 'Transport' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Transporte' AND c.kind = 'expense');

UPDATE categories SET name = 'Salud'
 WHERE name = 'Health' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Salud' AND c.kind = 'expense');

UPDATE categories SET name = 'Ocio'
 WHERE name = 'Leisure' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Ocio' AND c.kind = 'expense');

UPDATE categories SET name = 'Suscripciones'
 WHERE name = 'Subscriptions' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Suscripciones' AND c.kind = 'expense');

UPDATE categories SET name = 'Restaurantes'
 WHERE name = 'Eating out' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Restaurantes' AND c.kind = 'expense');

UPDATE categories SET name = 'Compras'
 WHERE name = 'Shopping' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Compras' AND c.kind = 'expense');

UPDATE categories SET name = 'Impuestos'
 WHERE name = 'Taxes' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Impuestos' AND c.kind = 'expense');

UPDATE categories SET name = 'Comisiones'
 WHERE name = 'Fees' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Comisiones' AND c.kind = 'expense');

UPDATE categories SET name = 'Otros gastos'
 WHERE name = 'Other expense' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Otros gastos' AND c.kind = 'expense');

UPDATE categories SET name = 'Traspaso'
 WHERE name = 'Transfer' AND is_system = 1
   AND NOT EXISTS (SELECT 1 FROM categories c WHERE c.name = 'Traspaso' AND c.kind = 'transfer');
