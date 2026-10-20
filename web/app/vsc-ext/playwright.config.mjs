export default {
  testDir: "./tests/browser",
  use: {
    baseURL: "http://markview.test",
    browserName: "chromium",
    launchOptions: { args: ["--enable-unsafe-swiftshader"] },
  },
};
