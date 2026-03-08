import std.stdio;
import std.math : sqrt, pow;
import std.algorithm : filter, reduce;

struct Point {
    double x;
    double y;
}

class Shape {
    string name;

    this(string name) {
        this.name = name;
    }

    double area() {
        return 0.0;
    }
}

class Circle : Shape {
    double radius;

    this(double r) {
        super("circle");
        this.radius = r;
    }

    override double area() {
        return 3.14159 * radius * radius;
    }
}

double distance(Point a, Point b) {
    double dx = b.x - a.x;
    double dy = b.y - a.y;
    return sqrt(dx * dx + dy * dy);
}

string classify(int n) {
    if (n < 0) {
        return "negative";
    } else if (n == 0) {
        return "zero";
    } else {
        return "positive";
    }
}

int sumEvens(int[] values) {
    int total = 0;
    foreach (v; values) {
        if (v % 2 == 0) {
            total += v;
        }
    }
    return total;
}

void main() {
    auto p1 = Point(3.0, 4.0);
    auto p2 = Point(0.0, 0.0);
    writeln(distance(p1, p2));

    auto c = new Circle(5.0);
    writeln(c.area());
    writeln(classify(-3));
    writeln(sumEvens([1, 2, 3, 4, 5, 6]));
}
