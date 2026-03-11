package users

import (
	"context"
	"database/sql"
)

// context.TODO() as a placeholder — should be replaced with threaded context
func GetUser(db *sql.DB, id string) (*User, error) {
	row := db.QueryRowContext(context.TODO(), "SELECT * FROM users WHERE id = ?", id)
	var u User
	if err := row.Scan(&u.ID, &u.Name); err != nil {
		return nil, err
	}
	return &u, nil
}

// context.TODO() in a nested call
func ListUsers(db *sql.DB) ([]*User, error) {
	rows, err := db.QueryContext(context.TODO(), "SELECT * FROM users")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	return scanUsers(rows)
}
