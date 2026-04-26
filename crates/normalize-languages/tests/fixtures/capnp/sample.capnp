@0xdbb9ad1f14bf0b36;

using Cxx = import "/capnp/c++.capnp";

# A point in 2D space
struct Point {
  x @0 :Float64;
  y @1 :Float64;
}

# A person with a name and email
struct Person {
  name @0 :Text;
  email @1 :Text;
  age @2 :UInt32;
}

struct AddressBook {
  people @0 :List(Person);
}

# Interface for a calculator service
interface Calculator {
  add @0 (left :Float64, right :Float64) -> (value :Float64);
  subtract @1 (left :Float64, right :Float64) -> (value :Float64);
}
