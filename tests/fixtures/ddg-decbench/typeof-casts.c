typedef unsigned short u16;
typedef unsigned char u8;
/* u-boot's get_unaligned helpers: the declared name is in scope in its own initializer. */
static inline u16 get_be16(const void *p)
{
	return ({ const struct { u16 x; } __attribute__((packed)) *__pptr = (typeof(__pptr))(p); __pptr->x; });
}
int own_initializer(const void *p)
{
	const u8 *q = (typeof(q))(p);
	int v = (__typeof__(v))(*q);
	return v + ((typeof(v))p == 0);
}
int earlier_names(const void *p)
{
	const u8 *q;
	const u8 *r = (typeof(q))(p);
	int a = 1, b = (typeof(a))(*r);
	return b;
}
int unbound_name(const void *p)
{
	int b = (typeof(nothing))(p);
	return b + (typeof(b))(p);
}
int statement_expression(const void *p)
{
	int outer = ({ int inner = (typeof(inner))(p); inner; });
	return outer;
}
