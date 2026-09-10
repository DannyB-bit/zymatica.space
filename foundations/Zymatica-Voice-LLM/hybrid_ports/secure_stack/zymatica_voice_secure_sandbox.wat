(module
  ;; Watermark: ip zymatica.space | astronautshe.com
  ;; Copyright (c) 2026 Zymatica. All rights reserved.
  (memory 1)
  (func $safe_parse (param $ptr i32) (param $len i32) (result i32)
    local.get $ptr
    i32.load
  )
  (export "safe_parse" (func $safe_parse))
)
