CREATE TABLE numbers (
    id    INTEGER PRIMARY KEY,
    value INTEGER NOT NULL,
    label TEXT
);

CREATE TABLE results (
    id         INTEGER PRIMARY KEY,
    operation  TEXT NOT NULL,
    operand_a  INTEGER NOT NULL,
    operand_b  INTEGER NOT NULL,
    result     INTEGER NOT NULL
);

CREATE VIEW number_summary AS
SELECT label, value
FROM numbers
ORDER BY value;
