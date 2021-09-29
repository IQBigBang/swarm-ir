(module
  (type $t1 (func (param i32) (result i32)))
  (func $f (type $t1) (param $p0 i32) (result i32)
    (if (result i32)
        (i32.lt_s
            (get_local $p0)
            (i32.const 0)
        )
        (then
            (i32.const 42)
        )
        (else
            (i32.mul
                (get_local $p0)
                (i32.const 3)
            ) 
        )
    ))
)