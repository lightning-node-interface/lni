const createNoopExpectation = () =>
  new Proxy(
    {},
    {
      get: () => (..._args) => createNoopExpectation(),
      apply: () => createNoopExpectation(),
    },
  );

export function expect(_value) {
  return createNoopExpectation();
}

export default {
  expect,
};
