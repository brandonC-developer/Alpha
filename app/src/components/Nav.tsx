import { NavLink } from "react-router";

const RUTAS = [
  { to: "/", label: "Panel" },
  { to: "/costes", label: "Costes" },
  { to: "/planes", label: "Planes" },
  { to: "/sesiones", label: "Sesiones" },
  { to: "/fuentes", label: "Fuentes" },
  { to: "/ajustes", label: "Ajustes" },
] as const;

export function Nav() {
  return (
    <aside className="sidebar">
      <div className="sidebar-title">Gestor de Tokens</div>
      <nav aria-label="Navegación principal">
        {RUTAS.map((ruta) => (
          <NavLink
            key={ruta.to}
            to={ruta.to}
            end={ruta.to === "/"}
            className={({ isActive }) =>
              isActive ? "nav-link active" : "nav-link"
            }
          >
            {ruta.label}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
