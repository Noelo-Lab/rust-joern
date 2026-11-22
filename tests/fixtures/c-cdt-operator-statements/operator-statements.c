unsigned int operator()(unsigned long *slot, unsigned long step)
{
  if (step)
    return slot[0] + step;
  return 0;
}

long *reserve(long count)
{
  long *block;
  if (count > 0) {
    block = operator new(count * 8);
    record(block);
    return block;
  }
  fail();
  return 0;
}

void refill(int ready)
{
  long *buffer;
  if (!ready)
    goto again;
  prepare();
again:
  buffer = (long *)operator new(64);
  consume(buffer);
}

void pick(int wide)
{
  long *slot;
  slot = wide ? (long *)operator new() : 0;
  finish(slot);
}

void choose(int flag)
{
  int value;
  value = flag ? first second() : 0;
  finish(value);
}

void closed(int flag)
{
  int value;
  value = flag ? first() : second();
  if (value)
    finish(value);
}
