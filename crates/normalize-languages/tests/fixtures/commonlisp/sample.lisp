(defpackage :sample
  (:use :cl)
  (:export #:greet #:factorial #:make-point))

(in-package :sample)

(require 'alexandria)
(use-package :iterate)

;; A simple struct
(defstruct point
  (x 0 :type integer)
  (y 0 :type integer))

;; A class definition
(defclass shape ()
  ((color :accessor shape-color :initarg :color :type string)
   (area  :accessor shape-area  :initform 0    :type number)))

;; Compute factorial
(defun factorial (n)
  (if (<= n 1)
      1
      (* n (factorial (- n 1)))))

;; Greet a person
(defun greet (name)
  (format t "Hello, ~a!~%" name))

;; Sum elements of a list
(defun sum-list (lst)
  (let ((total 0))
    (dolist (item lst)
      (setf total (+ total item)))
    total))

;; Find items matching a predicate
(defun filter-items (pred lst)
  (let ((result '()))
    (dolist (item lst)
      (when (funcall pred item)
        (push item result)))
    (reverse result)))

;; Generic function
(defgeneric describe-shape (shape)
  (:documentation "Return a description of the shape."))

(defmethod describe-shape ((s shape))
  (format nil "Shape with color ~a" (shape-color s)))
