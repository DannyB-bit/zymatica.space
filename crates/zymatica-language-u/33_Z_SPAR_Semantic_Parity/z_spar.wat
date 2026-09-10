;; ==============================================================================
;; Class 33: Z-SPAR (Zymatica Semantic Parity & Repair) - WebAssembly (WAT)
;; Author: Danny Bouldiez | Codebase by Devs One
;; License: SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
;; ==============================================================================

(module
  (memory (export "memory") 1)

  ;; GF(16) EXP Table at memory offset 0
  (data (i32.const 0) "\01\02\04\08\03\06\0c\0b\05\0a\07\0e\0f\0d\09\01\02\04\08\03\06\0c\0b\05\0a\07\0e\0f\0d\09\01\02")

  ;; GF(16) LOG Table at memory offset 32
  (data (i32.const 32) "\00\00\01\04\02\08\05\0a\03\0e\09\07\06\0d\0b\0c")

  ;; Function: gf16_add(a: i32, b: i32) -> i32
  (func $gf16_add (export "gf16_add") (param $a i32) (param $b i32) (result i32)
    (i32.and (i32.xor (local.get $a) (local.get $b)) (i32.const 15))
  )

  ;; Function: gf16_mul(a: i32, b: i32) -> i32
  (func $gf16_mul (export "gf16_mul") (param $a i32) (param $b i32) (result i32)
    (local $log_a i32)
    (local $log_b i32)
    (local $log_sum i32)

    (if (i32.or (i32.eqz (local.get $a)) (i32.eqz (local.get $b)))
      (then (return (i32.const 0)))
    )

    (local.set $log_a (i32.load8_u (i32.add (i32.const 32) (i32.and (local.get $a) (i32.const 15)))))
    (local.set $log_b (i32.load8_u (i32.add (i32.const 32) (i32.and (local.get $b) (i32.const 15)))))
    (local.set $log_sum (i32.rem_u (i32.add (local.get $log_a) (local.get $log_b)) (i32.const 15)))

    (i32.load8_u (local.get $log_sum))
  )
)
