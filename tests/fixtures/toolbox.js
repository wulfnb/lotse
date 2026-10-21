localStorage.setItem('fixture','stored value');
sessionStorage.setItem('temporary','session value');
performance.mark('fixture-ready');
window.fixtureRequests = async function fixtureRequests() {
  await fetch('/api/data?q=servo');
  await fetch('/api/echo',{method:'POST',headers:{'Content-Type':'application/json','X-Test':'Servo'},body:JSON.stringify({hello:'Servo'})});
  await fetch('/api/missing');
  await fetch('/api/redirect');
  document.querySelector('#output').textContent='Requests complete';
};
document.querySelector('#counter').onclick=fixtureRequests;
fixtureRequests();
