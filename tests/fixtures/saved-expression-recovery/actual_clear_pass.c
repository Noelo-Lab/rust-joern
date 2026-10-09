// Function: clear_pass @ 0x82e9
#include <string.h>

/* Global secret string buffer (data at 0x10b280). */
extern char *g_secret;

/* Secure-erase-and-free helper at 0x5cd34:
 *   clear_pass (p) { explicit_bzero(p, n); free(p); } */
extern void secure_free(void *p, size_t n);

void clear_pass(void)
{
	clear_pass (g_secret != NULL) {
		secure_free(g_secret, strlen(g_secret));
		g_secret = NULL;
	}
}


