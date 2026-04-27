(ns myapp.core
  (:require [clojure.string :as str]
            [clojure.set :as cset]))

; A point in 2D space with x and y coordinates
(defrecord Point [x y])

(defn distance
  "Compute Euclidean distance between two points."
  [p1 p2]
  (let [dx (- (:x p2) (:x p1))
        dy (- (:y p2) (:y p1))]
    (Math/sqrt (+ (* dx dx) (* dy dy)))))

(defn classify-point
  "Return a keyword describing which quadrant the point is in."
  [p]
  (cond
    (and (pos? (:x p)) (pos? (:y p))) :q1
    (and (neg? (:x p)) (pos? (:y p))) :q2
    (and (neg? (:x p)) (neg? (:y p))) :q3
    :else :q4))

^:deprecated
(defn sum-evens
  "Sum all even numbers in a collection."
  [coll]
  (reduce + 0
    (filter even? coll)))

(defmacro when-positive
  "Execute body when n is positive."
  [n & body]
  `(when (pos? ~n)
     ~@body))

(defn process-items
  "Process a sequence of items, returning results."
  [items]
  (for [item items
        :when (not (nil? item))]
    (str/trim (str item))))

(defn -main
  "Entry point."
  [& args]
  (let [p1 (->Point 3 4)
        p2 (->Point 0 0)]
    (println (distance p1 p2))
    (println (classify-point p1))
    (println (sum-evens [1 2 3 4 5 6]))))
