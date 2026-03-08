import 'dart:collection';
import 'dart:math' as math;

class Point {
  final double x;
  final double y;

  const Point(this.x, this.y);

  double distanceTo(Point other) {
    final dx = x - other.x;
    final dy = y - other.y;
    return math.sqrt(dx * dx + dy * dy);
  }
}

class Stack<T> {
  final Queue<T> _items = Queue<T>();

  void push(T item) {
    _items.addLast(item);
  }

  T? pop() {
    if (_items.isEmpty) return null;
    return _items.removeLast();
  }

  T? peek() {
    if (_items.isEmpty) return null;
    return _items.last;
  }

  bool get isEmpty => _items.isEmpty;
  int get length => _items.length;
}

String classify(int n) {
  if (n < 0) {
    return 'negative';
  } else if (n == 0) {
    return 'zero';
  } else {
    return 'positive';
  }
}

int sumEvens(List<int> numbers) {
  var total = 0;
  for (final n in numbers) {
    if (n % 2 == 0) total += n;
  }
  return total;
}

void main() {
  final stack = Stack<int>();
  stack.push(10);
  stack.push(20);
  print(stack.pop());
  print(classify(-3));
  print(sumEvens([1, 2, 3, 4, 5]));
  final p1 = Point(0.0, 0.0);
  final p2 = Point(3.0, 4.0);
  print(p1.distanceTo(p2));
}
