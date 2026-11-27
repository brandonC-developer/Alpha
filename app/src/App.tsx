import { BrowserRouter, Route, Routes } from "react-router";
import { Nav } from "./components/Nav";

function Placeholder({ title }: { title: string }) {
  return (
    <section>
      <h1>{title}</h1>
    </section>
  );
}

export function App() {
  return (
    <BrowserRouter>
      <div className="layout">
        <Nav />
        <main className="content">
          <Routes>
            <Route path="/" element={<Placeholder title="Panel" />} />
            <Route path="/costes" element={<Placeholder title="Costes" />} />
            <Route path="/planes" element={<Placeholder title="Planes" />} />
            <Route path="/sesiones" element={<Placeholder title="Sesiones" />} />
            <Route path="/fuentes" element={<Placeholder title="Fuentes" />} />
            <Route path="/ajustes" element={<Placeholder title="Ajustes" />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
}

export default App;
