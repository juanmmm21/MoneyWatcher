-- MoneyWatcher trabaja solo en euros: la divisa por cuenta era una ramificación
-- que no se usaba (selector del dashboard, filtro de las agregaciones y una
-- fila por divisa en el resumen por bancos) y que solo podía dar problemas.
ALTER TABLE accounts DROP COLUMN currency;
