import { expect, test } from "vitest";
import { render, screen } from "@testing-library/react";
import App from "../../src/App";

test("la app monta la navegación", () => {
  render(<App />);
  expect(screen.getByRole("navigation")).toBeInTheDocument();
});
