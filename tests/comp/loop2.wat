(module
  (type (;0;) (func (param i32) (result i32)))
  (func (;0;) (type 0) (param i32) (result i32)
    (local i32 i32)
    i32.const 0
    local.set 1
    i32.const 0
    local.set 2
    block  ;; label = @1
      loop  ;; label = @2
        local.get 2
        local.get 0
        i32.ge_s
        if  ;; label = @3
          br 2 (;@1;)
        else
        end
        local.get 1
        i32.const 2
        i32.add
        local.set 1
        local.get 2
        i32.const 1
        i32.add
        local.set 2
        br 0 (;@2;)
      end
    end
    local.get 1)
)