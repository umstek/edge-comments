addEventListener("fetch", (event) => {
  event.respondWith(handleRequest(event.request));
});

/**
 * Fetch and log a request
 * @param {Request} request
 */
async function handleRequest(request) {
  const { main } = wasm_bindgen;
  await wasm_bindgen(wasm);
  return await main(request);
}
