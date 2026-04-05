addEventListener("fetch", (event) => {
  event.respondWith(
    new Response("bootstrap", {
      status: 200,
      headers: { "content-type": "text/plain; charset=utf-8" },
    }),
  );
});
