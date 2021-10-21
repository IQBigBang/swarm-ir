(module 
    (type $t0 (func (result i32)))
    (func $g (type $t0)
        (local i32)
        (block 
            (loop
                local.get 0
                i32.const 1
                i32.add
                local.set 0
                br 0
            )
        )
        local.get 0
    )
)