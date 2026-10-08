struct Box { int set(int x) { return x; } };
int member_pointer(Box *b, int x) { int (Box::*m)(int) = &Box::set; return (b->*m)(x); }
int member_pointer_dot(Box b, int x) { int (Box::*m)(int) = &Box::set; return (b.*m)(x); }
