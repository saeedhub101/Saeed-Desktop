const tabs=[...document.querySelectorAll<HTMLButtonElement>("[data-tab]")];
const panels=[...document.querySelectorAll<HTMLElement>("[data-panel]")];

function activate(name:string){
  for(const tab of tabs){
    const active=tab.dataset.tab===name;
    tab.classList.toggle("active",active);
    tab.setAttribute("aria-selected",String(active));
  }
  for(const panel of panels) panel.hidden=panel.id!==name;
}

for(const tab of tabs) tab.addEventListener("click",()=>activate(tab.dataset.tab ?? "characters"));
activate("characters");
