from .models import Person

def main() -> None:
    p = Person("world")
    print(p.name)
