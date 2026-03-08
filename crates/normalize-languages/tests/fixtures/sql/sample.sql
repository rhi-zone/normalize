-- Sample SQL file with DDL and DML statements

CREATE SCHEMA inventory;

CREATE TABLE inventory.products (
    id          SERIAL PRIMARY KEY,
    name        VARCHAR(255) NOT NULL,
    price       NUMERIC(10, 2) NOT NULL,
    category    VARCHAR(100),
    stock       INTEGER DEFAULT 0,
    created_at  TIMESTAMP DEFAULT NOW()
);

CREATE TABLE inventory.orders (
    id          SERIAL PRIMARY KEY,
    product_id  INTEGER REFERENCES inventory.products(id),
    quantity    INTEGER NOT NULL,
    total_price NUMERIC(10, 2),
    ordered_at  TIMESTAMP DEFAULT NOW()
);

CREATE VIEW inventory.low_stock AS
    SELECT id, name, stock
    FROM inventory.products
    WHERE stock < 10
    ORDER BY stock ASC;

CREATE FUNCTION inventory.calculate_total(qty INTEGER, unit_price NUMERIC)
RETURNS NUMERIC AS $$
BEGIN
    RETURN qty * unit_price;
END;
$$ LANGUAGE plpgsql;

CREATE FUNCTION inventory.reorder_needed(product_id INTEGER)
RETURNS BOOLEAN AS $$
DECLARE
    current_stock INTEGER;
BEGIN
    SELECT stock INTO current_stock
    FROM inventory.products
    WHERE id = product_id;

    IF current_stock < 5 THEN
        RETURN TRUE;
    ELSE
        RETURN FALSE;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Query with JOIN and aggregation
SELECT
    p.name,
    COUNT(o.id) AS order_count,
    SUM(o.total_price) AS revenue
FROM inventory.products p
LEFT JOIN inventory.orders o ON p.id = o.product_id
WHERE p.category = 'electronics'
GROUP BY p.name
HAVING COUNT(o.id) > 0
ORDER BY revenue DESC;
