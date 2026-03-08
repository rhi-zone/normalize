(import (scheme base)
        (scheme write))

(define-record-type <point>
  (make-point x y)
  point?
  (x point-x)
  (y point-y))

(define (square n)
  (* n n))

(define (distance p1 p2)
  (let ((dx (- (point-x p2) (point-x p1)))
        (dy (- (point-y p2) (point-y p1))))
    (sqrt (+ (square dx) (square dy)))))

(define (classify n)
  (cond
    ((< n 0) 'negative)
    ((= n 0) 'zero)
    (else 'positive)))

(define (sum-evens lst)
  (let loop ((remaining lst) (acc 0))
    (cond
      ((null? remaining) acc)
      ((even? (car remaining))
       (loop (cdr remaining) (+ acc (car remaining))))
      (else
       (loop (cdr remaining) acc)))))

(define (map-values f lst)
  (if (null? lst)
      '()
      (cons (f (car lst))
            (map-values f (cdr lst)))))

(define (main)
  (let ((p1 (make-point 3 4))
        (p2 (make-point 0 0)))
    (display (distance p1 p2))
    (newline)
    (display (classify 5))
    (newline)
    (display (sum-evens '(1 2 3 4 5 6)))
    (newline)))

(main)
