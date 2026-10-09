int (standalone)(int x) { return x; }
int ((double_grouped))(int x) { return x; }
int (whole_parameters(int x)) { return x; }
int array[4];
int (*pointer_array(void))[4] { return &array; }
int target(int x) { return x; }
int (*callback_return(int x))(int) { return target; }
int simple_prototype(int);
int (simple_prototype)(int x) { return x; }
int (grouped_prototype)(int);
int (grouped_prototype)(int x) { return x; }
