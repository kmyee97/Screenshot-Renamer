import {render,screen,fireEvent,waitFor} from "@testing-library/react";
import {test,expect,vi,beforeEach} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {StartupSettings} from "./StartupSettings";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
beforeEach(()=>vi.mocked(invoke).mockResolvedValue(false));
test("shows the OS state and confirms a successful toggle",async()=>{
  render(<StartupSettings disabled={false} onSave={async()=>true}/>);
  const toggle=await screen.findByRole("checkbox",{name:"Start with Windows"});
  await waitFor(()=>expect(toggle).toBeEnabled());
  expect(toggle).not.toBeChecked();
  fireEvent.click(toggle);
  await waitFor(()=>expect(toggle).toBeChecked());
});
test("failed registration shows actual state and the failure",async()=>{
  render(<StartupSettings disabled={false} onSave={async()=>{throw "Startup registration failed.";}}/>);
  const toggle=await screen.findByRole("checkbox",{name:"Start with Windows"});
  await waitFor(()=>expect(toggle).toBeEnabled());
  fireEvent.click(toggle);
  expect(await screen.findByRole("alert")).toHaveTextContent("Startup registration failed.");
  await waitFor(()=>expect(toggle).toBeEnabled());
  expect(toggle).not.toBeChecked();
});
