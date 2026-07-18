const { chromium } = require("playwright");
const path = require("node:path");
const { pathToFileURL } = require("node:url");

let activeBrowser;

async function main() {
  const input = process.argv[2];
  if (!input) {
    throw new Error("usage: node script/test_timeline_ui.cjs <timeline.html>");
  }
  const absolute = path.resolve(input);
  const browser = await chromium.launch({
    headless: true,
    executablePath: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  });
  activeBrowser = browser;
  const page = await browser.newPage({ viewport: { width: 375, height: 812 } });
  const consoleErrors = [];
  const pageErrors = [];
  const remoteRequests = [];
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("request", (request) => {
    if (/^https?:/.test(request.url())) remoteRequests.push(request.url());
  });

  await page.goto(pathToFileURL(absolute).href, { waitUntil: "load" });
  await page
    .waitForFunction(
      () => document.getElementById("status")?.textContent?.includes("events shown"),
      null,
      { timeout: 2_000 },
    )
    .catch(() => {});
  if ((await page.locator(".event").count()) !== 2) {
    throw new Error("expected two timeline events");
  }
  const initialStatus = (await page.locator("#status").textContent())?.trim();
  if (initialStatus !== "2 of 2 events shown") {
    throw new Error(
      `initial event status is incorrect: ${JSON.stringify(initialStatus)}; console: ${consoleErrors.join(" | ")}; page: ${pageErrors.join(" | ")}`,
    );
  }

  await page.locator("#category").selectOption("inauguration");
  if ((await page.locator(".event:visible").count()) !== 1) {
    throw new Error("category filtering did not isolate one event");
  }
  await page.locator("#category").selectOption("");
  await page.locator("#search").fill("not in this timeline");
  if (!(await page.locator("#empty").isVisible())) {
    throw new Error("empty search state did not appear");
  }
  await page.locator("#search").fill("emancipation");
  if ((await page.locator(".event:visible").count()) !== 1) {
    throw new Error("text search did not isolate the matching event");
  }
  await page.locator("#search").fill("");

  const beforeSort = await page.locator(".event h2").first().textContent();
  await page.locator("#sort").click();
  const afterSort = await page.locator(".event h2").first().textContent();
  if (beforeSort === afterSort) {
    throw new Error("timeline sort control did not reorder events");
  }

  const downloadPromise = page.waitForEvent("download");
  await page.locator("#csv").click();
  const download = await downloadPromise;
  if (download.suggestedFilename() !== "inquiry-timeline.csv") {
    throw new Error("CSV export used an unexpected filename");
  }

  await page.locator("body").click({ position: { x: 2, y: 2 } });
  const focusOrder = [];
  for (let index = 0; index < 5; index += 1) {
    await page.keyboard.press("Tab");
    focusOrder.push(await page.evaluate(() => document.activeElement?.id || ""));
  }
  const expectedFocus = ["search", "category", "sort", "copy", "csv"];
  if (JSON.stringify(focusOrder) !== JSON.stringify(expectedFocus)) {
    throw new Error(`unexpected keyboard order: ${focusOrder.join(",")}`);
  }

  await page.screenshot({
    path: path.resolve("target/timeline-mobile.png"),
    fullPage: true,
  });
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.screenshot({
    path: path.resolve("target/timeline-desktop.png"),
    fullPage: true,
  });
  await page.setViewportSize({ width: 1440, height: 756 });
  await page.evaluate(() => {
    document.documentElement.style.zoom = "0.76";
  });
  await page.screenshot({
    path: path.resolve("target/timeline-blog.png"),
    fullPage: false,
  });
  await page.evaluate(() => {
    document.documentElement.style.zoom = "";
  });
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.emulateMedia({ colorScheme: "dark" });
  const darkColors = await page.locator(".event").first().evaluate((element) => {
    const styles = getComputedStyle(element);
    return { color: styles.color, background: styles.backgroundImage };
  });
  if (
    darkColors.color === "rgba(0, 0, 0, 0)" ||
    darkColors.background === "none"
  ) {
    throw new Error("dark-mode timeline tokens were not applied");
  }
  await page.screenshot({
    path: path.resolve("target/timeline-dark.png"),
    fullPage: true,
  });

  if (remoteRequests.length > 0) {
    throw new Error(`timeline made remote requests: ${remoteRequests.join(", ")}`);
  }
  if (consoleErrors.length > 0) {
    throw new Error(`browser console errors: ${consoleErrors.join(" | ")}`);
  }
  if (pageErrors.length > 0) {
    throw new Error(`browser page errors: ${pageErrors.join(" | ")}`);
  }
  await browser.close();
  activeBrowser = undefined;
  process.stdout.write(
    JSON.stringify(
      {
        events: 2,
        filters: "passed",
        sort: "passed",
        csv: "passed",
        keyboard: focusOrder,
        remote_requests: 0,
        console_errors: 0,
        screenshots: [
          "target/timeline-mobile.png",
          "target/timeline-desktop.png",
          "target/timeline-blog.png",
          "target/timeline-dark.png",
        ],
      },
      null,
      2,
    ) + "\n",
  );
}

main().catch(async (error) => {
  if (activeBrowser) {
    await activeBrowser.close().catch(() => {});
  }
  console.error(error);
  process.exitCode = 1;
});
