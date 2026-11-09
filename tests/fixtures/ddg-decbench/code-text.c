/* CODE text that reaches DDG labels: for headers and typeof specifiers in expanded built-ins. */
struct node { struct node *next; int v; };
int spaced(struct node *list, struct node *wl)
{
	struct node *l;
	int rval;
	for (rval = 0, l = wl ? wl : list ; l; l = l->next)
		rval += l->v;
	return rval;
}
int both_spaced(struct node *vp)
{
	int n = 0;
	for (n = 0 ; vp ; vp = vp->next)
		n++;
	for (int i = 0 ; n ; n--)
		vp = 0;
	for (n = 3
	     ; n; n--)
		vp = 0;
	for ( ; n ; )
		n--;
	return n;
}
typedef unsigned long size_t;
struct list_head { struct list_head *next; };
struct udevice { int seq; struct list_head sibling_node; union { int rb; } u; };
struct udevice *first_child(struct list_head *head)
{
	struct udevice *dev;
	dev = ({ const typeof( ((typeof(*dev) *)0)->sibling_node ) *__mptr = (head->next); (typeof(*dev) *)( (char *)__mptr - __builtin_offsetof(typeof(*dev), sibling_node) );});
	return dev;
}
size_t offsets(struct udevice *aeb)
{
	size_t a = __builtin_offsetof(__typeof__(*aeb), u.rb);
	size_t b = __builtin_offsetof(const typeof(aeb->u), rb);
	size_t c = __builtin_offsetof(struct udevice, seq);
	return a + b + c;
}
