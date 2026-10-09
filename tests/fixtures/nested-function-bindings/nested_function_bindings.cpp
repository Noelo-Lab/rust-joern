int sink(int (*callback)(int), int value);
int outer(int x, int y) {
 int local=x+1;
 int child(int z) {
  int shared=y+z;
  int grandchild(int x) { return x+y+z+local+shared; }
  sink(child,z);
  sink(grandchild,z);
  return x+y+z+local+grandchild(z);
 }
 sink(child,x);
 return child(x)+local;
}
int shadow_outer(int x, int y) {
 int local=x+1;
 int child_shadow(int x) { int local=x; return x+y+local; }
 return child_shadow(y)+local+x;
}
int visible_order(int x) {
 int before=x;
 int order_child(int z) { return x+before+after+z; }
 int after=x+2;
 return order_child(after);
}
int after_control(int x) { return x; }
