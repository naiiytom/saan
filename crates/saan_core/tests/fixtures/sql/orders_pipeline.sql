CREATE TABLE stg.orders AS
SELECT o.order_id, o.customer_id, o.amount
FROM raw.orders o
JOIN raw.customers c ON o.customer_id = c.id;

CREATE VIEW marts.order_summary AS
SELECT customer_id, SUM(amount) AS total
FROM stg.orders
GROUP BY customer_id;
