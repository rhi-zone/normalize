; Haskell tags query
; Covers: functions, data types, newtypes, type synonyms, typeclasses, instances

; Function definitions (bind + equation sets)
(function
  name: (variable) @name) @definition.function

; Type signatures (declarations without bodies)
(signature
  name: (variable) @name) @definition.function

; Data type declarations
(data_type
  name: (name) @name) @definition.class

; Newtype declarations
(newtype
  name: (name) @name) @definition.type

; Type synonym declarations
(type_synomym
  name: (name) @name) @definition.type

; Typeclass declarations (interfaces)
(class
  name: (name) @name) @definition.interface

; Instance declarations — captured as definition.module so extract_container
; can populate the implements list from the typeclass name.
(instance
  name: (name) @name) @definition.module
