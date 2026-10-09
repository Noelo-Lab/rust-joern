int known(int x);
int consume(int x,int y);
struct Object { int field; };
struct Object object;
struct Object *pointer;
int dotdot_statement(int x) { x=object..field; x++; return x; }
int dotdot_return(int x) { return object..field; x++; return x; }
int dotdot_initializer(int x) { int y=object..field; x++; return x; }
int dotdot_condition(int x) { if(object..field && x) known(x); return x; }
int dotdot_scalar_if(int x) { if(x) x=object..field; x++; return x; }
int dotdot_scalar_while(int x) { while(x) x=object..field; x++; return x; }
int dotdot_braced_if(int x) { if(x) { x=object..field; x++; } return x; }
int dotdot_argument(int x) { known(object..field); x++; return x; }
int trailing_dot_statement(int x) { x=object.; x++; return x; }
int trailing_dot_return(int x) { return object.; x++; return x; }
int trailing_dot_initializer(int x) { int y=object.; x++; return x; }
int trailing_dot_condition(int x) { if(object. && x) known(x); return x; }
int trailing_dot_scalar_if(int x) { if(x) x=object.; x++; return x; }
int trailing_dot_scalar_while(int x) { while(x) x=object.; x++; return x; }
int trailing_dot_braced_if(int x) { if(x) { x=object.; x++; } return x; }
int trailing_dot_argument(int x) { known(object.); x++; return x; }
int double_arrow_statement(int x) { x=object->->field; x++; return x; }
int double_arrow_return(int x) { return object->->field; x++; return x; }
int double_arrow_initializer(int x) { int y=object->->field; x++; return x; }
int double_arrow_condition(int x) { if(object->->field && x) known(x); return x; }
int double_arrow_scalar_if(int x) { if(x) x=object->->field; x++; return x; }
int double_arrow_scalar_while(int x) { while(x) x=object->->field; x++; return x; }
int double_arrow_braced_if(int x) { if(x) { x=object->->field; x++; } return x; }
int double_arrow_argument(int x) { known(object->->field); x++; return x; }
int empty_group_statement(int x) { x=(); x++; return x; }
int empty_group_return(int x) { return (); x++; return x; }
int empty_group_initializer(int x) { int y=(); x++; return x; }
int empty_group_condition(int x) { if(() && x) known(x); return x; }
int empty_group_scalar_if(int x) { if(x) x=(); x++; return x; }
int empty_group_scalar_while(int x) { while(x) x=(); x++; return x; }
int empty_group_braced_if(int x) { if(x) { x=(); x++; } return x; }
int empty_group_argument(int x) { known(()); x++; return x; }
int negated_empty_statement(int x) { x=!(); x++; return x; }
int negated_empty_return(int x) { return !(); x++; return x; }
int negated_empty_initializer(int x) { int y=!(); x++; return x; }
int negated_empty_condition(int x) { if(!() && x) known(x); return x; }
int negated_empty_scalar_if(int x) { if(x) x=!(); x++; return x; }
int negated_empty_scalar_while(int x) { while(x) x=!(); x++; return x; }
int negated_empty_braced_if(int x) { if(x) { x=!(); x++; } return x; }
int negated_empty_argument(int x) { known(!()); x++; return x; }
int separator_comparison_assignment(int x) { known(x!=0) x=known(x); x++; return x; }
int separator_identifier_assignment(int x) { known(x) x=known(x); x++; return x; }
int separator_comparison_call(int x) { known(x!=0) known(x); x++; return x; }
int separator_identifier_call(int x) { known(x) known(x); x++; return x; }
int separator_grouped_call(int x) { known((x)) known(x); x++; return x; }
int separator_comparison_declaration(int x) { known(x!=0) int y=x; return y; }
int separator_identifier_declaration(int x) { known(x) int y=x; return y; }
int separator_comparison_if(int x) { known(x!=0) if(x) known(x); return x; }
int separator_identifier_if(int x) { known(x) if(x) known(x); return x; }
int separator_comparison_while(int x) { known(x!=0) while(x) known(x); return x; }
int separator_identifier_while(int x) { known(x) while(x) known(x); return x; }
int separator_unknown_assignment(int x) { mystery(x) x=known(x); return x; }
int separator_unknown_comparison_assignment(int x) { mystery(x!=0) x=known(x); return x; }
int separator_call_block(int x) { known(x) { known(x); } return x; }
int valid_semicolon_assignment(int x) { known(x!=0); x=known(x); x++; return x; }
int valid_semicolon_declaration(int x) { known(x); int y=x; return y; }
int valid_semicolon_if(int x) { known(x); if(x) known(x); return x; }
int valid_empty_call(int x) { mystery(); return x; }
int valid_grouped_calls(int x) { consume((x),(known(x))); return x; }
int valid_field_access(int x) { x=object.field+pointer->field; return x; }
int valid_offsetof(int x) { x=__builtin_offsetof(struct Object,field); return x; }
int valid_cast_group(int x) { x=(int)(x+known(x)); return x; }
int valid_cpp_functional_cast(int x) { int y=int(x); return int(); }
