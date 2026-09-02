-- MoneyWatcher registra movimientos, no saldos: el saldo de apertura obligaba
-- al usuario a copiar a mano una cifra del banco para que las sumas
-- significaran algo, y quedaba desfasada en cuanto el extracto no empezaba
-- justo donde acababa el anterior. El saldo que trae cada línea del extracto
-- sigue guardándose en `transactions.balance_after`, pero solo se usa para
-- comprobar que el fichero importado se ha entendido bien.
ALTER TABLE accounts DROP COLUMN opening_balance;
