// Thrift IDL sample file

namespace py sample
namespace java com.example.sample

include "shared.thrift"

// A unique identifier type
typedef string UUID

// User account status
enum Status {
  ACTIVE = 1,
  INACTIVE = 2,
  BANNED = 3,
}

// A user in the system
struct User {
  1: required UUID id,
  2: required string name,
  3: required string email,
  4: optional Status status = Status.ACTIVE,
}

// Exception thrown when user is not found
exception UserNotFound {
  1: string message,
}

// Service for managing users
service UserService {
  // Retrieve a user by ID
  User getUser(1: UUID id) throws (1: UserNotFound notFound),
  list<User> listUsers(),
  void deleteUser(1: UUID id) throws (1: UserNotFound notFound),
}
