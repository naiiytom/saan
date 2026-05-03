CREATE TABLE marts.customer_stats AS
WITH active AS (
    SELECT id FROM raw.customers WHERE status = 'active'
),
counts AS (
    SELECT customer_id, COUNT(*) AS n FROM raw.orders GROUP BY customer_id
)
SELECT a.id, c.n
FROM active a
JOIN counts c ON a.id = c.customer_id;
