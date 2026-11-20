// Run from the repository root. Deliberately bound to loopback only.
const root = `${process.cwd()}/experiments/gesture-input`;
const allowed = new Map([
  ['/experiments/gesture-input/', ['index.html', 'text/html; charset=utf-8']],
  ['/experiments/gesture-input/gesture-input.mjs', ['gesture-input.mjs', 'text/javascript; charset=utf-8']],
]);
Bun.serve({hostname: '127.0.0.1', port: 47892, fetch(request) {
  const entry = allowed.get(new URL(request.url).pathname);
  return entry ? new Response(Bun.file(`${root}/${entry[0]}`), {headers: {'Content-Type': entry[1]}}) : new Response('Not found', {status: 404});
}});
console.log('Gesture diagnostic: http://127.0.0.1:47892/experiments/gesture-input/');
