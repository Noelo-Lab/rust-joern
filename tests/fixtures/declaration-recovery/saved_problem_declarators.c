int work(int);
int problem_type(int n) { long() ** value; work(n); return n; }
int after_problem_type(int n) { work(n); return n; }
int problem_type_argument(int n) { long(int) ** value; work(n); return n; }
int after_problem_type_argument(int n) { work(n); return n; }
long int64_t duplicate_parameter_list(int32_t *arg)(int *arg) { long() ** value; work(1); return 1; }
int after_duplicate_parameter_list(int n) { work(n); return n; }
define pseudo_function
{
    ptr32 %continuation;
    ptr32 lr = %continuation;
    work(1);
}
int after_pseudo_function(int n) { work(n); return n; }
other bare_body { work(1); }
int after_bare_body(int n) { work(n); return n; }
int valid_pointer_type(int n) { long (**value)(int); work(n); return n; }
int after_valid_pointer_type(int n) { work(n); return n; }
