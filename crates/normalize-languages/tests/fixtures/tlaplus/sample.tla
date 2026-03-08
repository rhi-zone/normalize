---- MODULE Sample ----
EXTENDS Naturals, Sequences, TLC

CONSTANTS MaxCount, InitValue

VARIABLES counter, stack, active

(* Type invariant *)
TypeInvariant ==
    /\ counter \in Nat
    /\ stack \in Seq(Nat)
    /\ active \in BOOLEAN

(* Initial state *)
Init ==
    /\ counter = InitValue
    /\ stack = << >>
    /\ active = TRUE

(* Increment the counter *)
Increment ==
    /\ active = TRUE
    /\ counter < MaxCount
    /\ counter' = counter + 1
    /\ UNCHANGED << stack, active >>

(* Push a value onto the stack *)
Push(val) ==
    /\ active = TRUE
    /\ stack' = Append(stack, val)
    /\ UNCHANGED << counter, active >>

(* Pop the top value from the stack *)
Pop ==
    /\ Len(stack) > 0
    /\ stack' = SubSeq(stack, 1, Len(stack) - 1)
    /\ UNCHANGED << counter, active >>

(* Deactivate the system *)
Deactivate ==
    /\ active = TRUE
    /\ active' = FALSE
    /\ UNCHANGED << counter, stack >>

(* Next state relation *)
Next ==
    \/ Increment
    \/ Push(counter)
    \/ Pop
    \/ Deactivate

(* Safety: counter never exceeds MaxCount *)
Safety == counter <= MaxCount

(* Liveness: system eventually becomes inactive *)
Liveness == <>(active = FALSE)

Spec == Init /\ [][Next]_<<counter, stack, active>>

====
