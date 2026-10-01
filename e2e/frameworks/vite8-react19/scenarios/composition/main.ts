const routes = {
  a: () => import("./a"),
  b: () => import("./b"),
};

window.goto = async (name) => {
  const { props } = await routes[name]();
  document.querySelector("#sample").className = props.className;
  window.current = name;
};
window.goto(new URL(location.href).searchParams.get("route") ?? "a");
