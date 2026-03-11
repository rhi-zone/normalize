package users

import (
	"context"
	"database/sql"
)

// Proper context threading — ctx comes from the caller
func GetUser(ctx context.Context, db *sql.DB, id string) (*User, error) {
	row := db.QueryRowContext(ctx, "SELECT * FROM users WHERE id = ?", id)
	var u User
	if err := row.Scan(&u.ID, &u.Name); err != nil {
		return nil, err
	}
	return &u, nil
}

// context.Background() is appropriate at entry points
func StartServer() {
	ctx := context.Background()
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()
	run(ctx)
}
