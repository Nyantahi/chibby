import { Routes, Route } from 'react-router-dom';
import Layout from './components/Layout';
import Projects from './components/Projects';
import ProjectDetail from './components/ProjectDetail';
import AddProject from './components/AddProject';
import RunDetail from './components/RunDetail';
import Settings from './components/Settings';
import Insights from './components/Insights';
import Templates from './components/Templates';
import CrashLog from './components/CrashLog';
import AgentDrawer from './components/AgentDrawer';

function App() {
  return (
    <>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<Projects />} />
          <Route path="projects" element={<Projects />} />
          <Route path="project/:projectId" element={<ProjectDetail />} />
          <Route path="add-project" element={<AddProject />} />
          <Route path="run/:runId" element={<RunDetail />} />
          <Route path="insights" element={<Insights />} />
          <Route path="templates" element={<Templates />} />
          <Route path="settings" element={<Settings />} />
          <Route path="crashes" element={<CrashLog />} />
        </Route>
      </Routes>
      <AgentDrawer />
    </>
  );
}

export default App;
