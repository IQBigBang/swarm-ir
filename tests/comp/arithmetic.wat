(module
  (type $t1 (func (param i32 i32) (result i32)))
  (func $f (type $t1) (param $p0 i32) (param $p1 i32) (result i32)
    get_local $p0
    get_local $p1
    i32.add)
)