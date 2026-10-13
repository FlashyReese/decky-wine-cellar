export const log = (...args: unknown[]) => {
  console.log(
    `%c Decky %c Wine Cellar %c`,
    "background: #16a085; color: black;",
    "background: #1abc9c; color: black;",
    "background: transparent;",
    ...args,
  );
};

export const debug = (...args: unknown[]) => {
  console.debug(
    `%c Decky %c Wine Cellar %c`,
    "background: #16a085; color: black;",
    "background: #1abc9c; color: black;",
    "color: blue;",
    ...args,
  );
};

export const error = (...args: unknown[]) => {
  console.error(
    `%c Decky %c Wine Cellar %c`,
    "background: #16a085; color: black;",
    "background: #FF0000;",
    "background: transparent;",
    ...args,
  );
};
