; Idris tags query

; Function signatures: distance : Point -> Point -> Double
(signature
  name: (loname) @name) @definition.function

; Data types: data Shape = ...
(data
  name: (data_name) @name) @definition.class

; Records: record Point where ...
(record
  name: (record_name) @name) @definition.class

; Interfaces
(interface
  (interface_head
    name: (interface_name) @name)) @definition.interface
